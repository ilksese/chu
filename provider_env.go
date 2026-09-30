package main

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strconv"
	"strings"

	"github.com/joho/godotenv"
)

func readProviderEnv(root string) ([]byte, bool, error) {
	path := filepath.Join(root, ".env")
	info, err := os.Lstat(path)
	if errors.Is(err, os.ErrNotExist) {
		return nil, false, nil
	}
	if err != nil || !info.Mode().IsRegular() {
		return nil, false, errors.New("无法读取凭证文件：必须是普通文件")
	}
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, false, errors.New("无法读取凭证文件")
	}
	return data, true, nil
}

func providerKey(root, name string) (string, error) {
	if name == "" {
		return "", nil
	}
	data, _, err := readProviderEnv(root)
	if err != nil {
		return "", err
	}
	values, err := godotenv.Unmarshal(string(data))
	if err != nil {
		return "", errors.New("凭证文件格式无效")
	}
	key, defined := values[name]
	if !defined {
		for variable, value := range values {
			if sameProviderEnv(variable, name) {
				if defined && key != value {
					return "", errors.New("凭证文件包含大小写冲突的环境变量")
				}
				key, defined = value, true
			}
		}
	}
	if !defined {
		key, defined = os.LookupEnv(name)
	}
	if !defined || key == "" {
		return "", errors.New("API Key 环境变量未定义或为空")
	}
	return key, nil
}

func providerEnvUpdate(original []byte, name, key string) ([]byte, error) {
	if _, err := godotenv.Unmarshal(string(original)); err != nil {
		return nil, errors.New("凭证文件格式无效，未写入")
	}
	assignment, err := godotenv.Marshal(map[string]string{name: key})
	if err != nil {
		return nil, errors.New("无法编码 API Key")
	}
	// godotenv.Marshal formats integer strings numerically, losing leading zeros.
	if _, err := strconv.Atoi(key); err == nil {
		assignment = name + "=\"" + key + "\""
	}
	var output strings.Builder
	record, replaced := "", false
	newline := "\n"
	if strings.Contains(string(original), "\r\n") {
		newline = "\r\n"
	}
	// Let godotenv identify complete records, including quoted multiline values.
	for _, line := range strings.SplitAfter(string(original), "\n") {
		record += line
		values, err := godotenv.Unmarshal(record)
		if err != nil {
			continue
		}
		target := ""
		for variable := range values {
			if sameProviderEnv(variable, name) {
				target = variable
				break
			}
		}
		if target == "" {
			output.WriteString(record)
		} else {
			if len(values) != 1 {
				return nil, errors.New("目标凭证变量必须独占一个逻辑记录，未写入")
			}
			ending := ""
			if strings.HasSuffix(record, "\r\n") {
				ending = "\r\n"
			} else if strings.HasSuffix(record, "\n") {
				ending = "\n"
			}
			comment := providerEnvComment(strings.TrimSuffix(record, ending), target, values[target])
			if !replaced {
				output.WriteString(assignment)
				if comment != "" {
					output.WriteByte(' ')
					output.WriteString(comment)
				}
				output.WriteString(ending)
				replaced = true
			} else if comment != "" {
				output.WriteString(comment + ending)
			}
		}
		record = ""
	}
	if record != "" {
		return nil, errors.New("凭证文件逻辑记录不完整，未写入")
	}
	if !replaced {
		if output.Len() > 0 && !strings.HasSuffix(output.String(), "\n") {
			output.WriteString(newline)
		}
		output.WriteString(assignment + newline)
	}
	updated := []byte(output.String())
	values, err := godotenv.Unmarshal(string(updated))
	if err != nil || values[name] != key {
		return nil, errors.New("API Key 无法无损编码，未写入")
	}
	return updated, nil
}

func providerEnvComment(record, name, value string) string {
	for i := 0; i < len(record); i++ {
		if record[i] != '#' {
			continue
		}
		// Parsing the prefix keeps hashes inside quoted multiline values out of comments.
		prefix, err := godotenv.Unmarshal(record[:i])
		spaceBefore := len(strings.TrimSpace(record[:i])) < len(record[:i])
		if parsed, ok := prefix[name]; err == nil && len(prefix) == 1 && ok && (parsed == value || spaceBefore) {
			return record[i:]
		}
	}
	return ""
}

func stageProviderFile(path string, data []byte) (string, error) {
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		return "", err
	}
	f, err := os.CreateTemp(filepath.Dir(path), ".chu-provider-")
	if err != nil {
		return "", err
	}
	name := f.Name()
	defer func() {
		_ = f.Close()
		if err != nil {
			_ = os.Remove(name)
		}
	}()
	if err = f.Chmod(0o600); err == nil {
		_, err = f.Write(data)
	}
	if err == nil {
		err = f.Sync()
	}
	if closeErr := f.Close(); err == nil {
		err = closeErr
	}
	return name, err
}

func saveProviderCredential(root, name, key string, state []byte) error {
	original, exists, err := readProviderEnv(root)
	if err != nil {
		return err
	}
	updated, err := providerEnvUpdate(original, name, key)
	if err != nil {
		return err
	}
	envPath, statePath := filepath.Join(root, ".env"), filepath.Join(root, "state.json")
	envTemp, err := stageProviderFile(envPath, updated)
	if err != nil {
		return fmt.Errorf("保存凭证失败: %w", err)
	}
	defer os.Remove(envTemp)
	stateTemp, err := stageProviderFile(statePath, state)
	if err != nil {
		return err
	}
	defer os.Remove(stateTemp)
	backup := ""
	if exists {
		backup, err = stageProviderFile(envPath, original)
		if err != nil {
			return err
		}
		defer func() {
			if backup != "" {
				_ = os.Remove(backup)
			}
		}()
	}
	if err := os.Rename(envTemp, envPath); err != nil {
		return fmt.Errorf("保存凭证失败: %w", err)
	}
	if err := os.Rename(stateTemp, statePath); err != nil {
		var rollback error
		if exists {
			rollback = os.Rename(backup, envPath)
		} else {
			rollback = os.Remove(envPath)
		}
		if rollback != nil {
			// Preserve the staged original if the filesystem also prevents rollback.
			backup = ""
			return fmt.Errorf("保存状态失败且凭证回滚失败: %w", errors.Join(err, rollback))
		}
		return err
	}
	return nil
}
